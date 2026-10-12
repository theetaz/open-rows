// SPDX-License-Identifier: Apache-2.0
/** Validated JSON document input; EJSON type validation also runs before review. */
export function DocumentField(props: {
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}): React.JSX.Element {
  const error = documentError(props.value);
  return (
    <>
      <textarea
        aria-label="Edit __document"
        rows={8}
        value={props.value}
        disabled={props.disabled}
        aria-invalid={Boolean(error)}
        onChange={(event) => {
          props.onChange(event.target.value);
        }}
      />
      {error && !props.disabled && <p role="alert">{error}</p>}
    </>
  );
}
function documentError(text: string): string | null {
  try {
    const value: unknown = JSON.parse(text);
    return value !== null && typeof value === 'object' && !Array.isArray(value)
      ? null
      : 'Use a JSON/EJSON object.';
  } catch {
    return 'Enter valid JSON/EJSON. Extended types use $oid, $date and $numberLong.';
  }
}
