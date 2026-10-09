// SPDX-License-Identifier: Apache-2.0
//! Prebuilt name search over qualified object names (sidebar search, quick
//! open, SQL completion). Lower-cased keys are built once per index, so a
//! query is one linear pass of `starts_with` / `contains` over short strings
//! — a few milliseconds for 50 000 objects, far below the 50 ms budget.

use openrows_driver_api::catalog::ObjectSummary;

/// The bounded name index of one database.
#[derive(Debug)]
pub struct NameIndex {
    objects: Vec<ObjectSummary>,
    /// Lower-cased `name`, per object.
    names: Vec<String>,
    /// Lower-cased `schema.name`, per object.
    qualified: Vec<String>,
}

/// One search over a [`NameIndex`].
#[derive(Clone, Copy, Debug)]
pub struct SearchQuery<'a> {
    /// Case-insensitive text; empty lists everything in index order.
    pub text: &'a str,
    /// Only objects of this schema, when set.
    pub schema: Option<&'a str>,
    /// Most objects returned.
    pub limit: usize,
}

/// The best matches of a search and how many objects matched in all.
#[derive(Debug)]
pub struct SearchHits<'a> {
    /// Best first: exact name, name prefix, qualified prefix, name
    /// substring, qualified substring, name subsequence; index order
    /// within each rank.
    pub objects: Vec<&'a ObjectSummary>,
    /// Every match, including those beyond the limit.
    pub total: usize,
}

impl NameIndex {
    /// Index `objects` (kept in the given order).
    #[must_use]
    pub fn new(objects: Vec<ObjectSummary>) -> Self {
        let names = objects
            .iter()
            .map(|object| object.name.to_lowercase())
            .collect();
        let qualified = objects
            .iter()
            .map(|object| format!("{}.{}", object.schema, object.name).to_lowercase())
            .collect();
        Self {
            objects,
            names,
            qualified,
        }
    }

    /// The indexed objects in index order.
    #[must_use]
    pub fn objects(&self) -> &[ObjectSummary] {
        &self.objects
    }

    /// Ranked matches of `query`.
    #[must_use]
    pub fn search(&self, query: &SearchQuery<'_>) -> SearchHits<'_> {
        let needle = query.text.trim().to_lowercase();
        let mut ranked: Vec<(u8, usize)> = self
            .objects
            .iter()
            .zip(self.names.iter().zip(&self.qualified))
            .enumerate()
            .filter(|(_, (object, _))| query.schema.is_none_or(|schema| object.schema == schema))
            .filter_map(|(position, (_, (name, qualified)))| {
                Some((rank(name, qualified, &needle)?, position))
            })
            .collect();
        let total = ranked.len();
        if query.limit < total {
            ranked.select_nth_unstable(query.limit);
            ranked.truncate(query.limit);
        }
        ranked.sort_unstable();
        SearchHits {
            objects: ranked
                .into_iter()
                .filter_map(|(_, position)| self.objects.get(position))
                .collect(),
            total,
        }
    }
}

/// Lower is better; `None` when the object does not match.
fn rank(name: &str, qualified: &str, needle: &str) -> Option<u8> {
    if needle.is_empty() {
        return Some(0);
    }
    if name == needle {
        Some(0)
    } else if name.starts_with(needle) {
        Some(1)
    } else if qualified.starts_with(needle) {
        Some(2)
    } else if name.contains(needle) {
        Some(3)
    } else if qualified.contains(needle) {
        Some(4)
    } else if subsequence(name, needle) {
        Some(5)
    } else {
        None
    }
}

/// Every character of `needle` appears in `name` in order (`cstm` finds
/// `customers`), the loose match quick open keeps from fuzzy search.
fn subsequence(name: &str, needle: &str) -> bool {
    let mut rest = name.chars();
    needle
        .chars()
        .all(|wanted| rest.any(|found| found == wanted))
}

#[cfg(test)]
mod tests {
    use openrows_driver_api::catalog::ObjectKind;

    use super::*;

    fn object(schema: &str, name: &str) -> ObjectSummary {
        ObjectSummary {
            schema: schema.to_owned(),
            name: name.to_owned(),
            kind: ObjectKind::Table,
            arguments: None,
            row_estimate: None,
            comment: None,
        }
    }

    fn names(hits: &SearchHits<'_>) -> Vec<String> {
        hits.objects
            .iter()
            .map(|object| format!("{}.{}", object.schema, object.name))
            .collect()
    }

    fn index() -> NameIndex {
        NameIndex::new(vec![
            object("public", "customer_orders"),
            object("public", "orders"),
            object("sales", "Orders_2024"),
            object("orders", "items"),
            object("public", "users"),
        ])
    }

    #[test]
    fn ranks_exact_then_prefix_then_qualified_then_substring() {
        let index = index();
        let hits = index.search(&SearchQuery {
            text: "ORDERS",
            schema: None,
            limit: 10,
        });
        assert_eq!(
            names(&hits),
            [
                "public.orders",
                "sales.Orders_2024",
                "orders.items",
                "public.customer_orders"
            ]
        );
        assert_eq!(hits.total, 4);
    }

    #[test]
    fn qualified_names_schema_filter_and_limit() {
        let index = index();
        let qualified = index.search(&SearchQuery {
            text: "public.ord",
            schema: None,
            limit: 10,
        });
        assert_eq!(names(&qualified), ["public.orders"]);
        let loose = index.search(&SearchQuery {
            text: "cstord",
            schema: None,
            limit: 10,
        });
        assert_eq!(names(&loose), ["public.customer_orders"]);
        let limited = index.search(&SearchQuery {
            text: "",
            schema: Some("public"),
            limit: 2,
        });
        assert_eq!(names(&limited), ["public.customer_orders", "public.orders"]);
        assert_eq!(limited.total, 3);
    }

    #[test]
    fn limit_keeps_the_best_matches() {
        let mut objects: Vec<ObjectSummary> = (0..1_000)
            .map(|n| object("public", &format!("t_{n}_orders")))
            .collect();
        objects.push(object("public", "orders"));
        let index = NameIndex::new(objects);
        let hits = index.search(&SearchQuery {
            text: "orders",
            schema: None,
            limit: 3,
        });
        assert_eq!(
            names(&hits),
            ["public.orders", "public.t_0_orders", "public.t_1_orders"]
        );
        assert_eq!(hits.total, 1_001);
    }
}
