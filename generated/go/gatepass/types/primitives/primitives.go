// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package primitives spells the specification's primitives for this target.
//
// Four map onto types that already mean exactly the same thing: `String` stays `string`,
// `Boolean` is `bool`, `Integer` is `int64`, `Bytes` is `[]byte`. The four below have no
// standard-library equivalent, and no dependency is taken for them — this module builds from
// exactly its committed bytes, with nothing to download.
package primitives

// Decimal is an exact decimal, carried as its wire rendering — a decimal string such as `10.50`.
// Never a float: money does not round the way a float does, and arithmetic is deliberately
// absent, because what a decimal *does* is behaviour.
//
// A wrapper over its wire rendering, distinct from `string` and from every other wrapper here for
// the reason the specification's own newtypes are distinct from their representations: a value's
// meaning is not its shape. The field is unexported, so the only way to make one is
// [NewDecimal] — but Go's zero value needs no constructor, so `Decimal{}` is still
// spellable (see TARGET.md).
type Decimal struct {
	value string
}

// NewDecimal wraps a decimal string as a Decimal.
func NewDecimal(value string) Decimal {
	return Decimal{value: value}
}

// Value is the wrapped rendering.
func (v Decimal) Value() string {
	return v.value
}

// Duration is a length of time, carried as its wire rendering — an ISO 8601 duration such as `P30D`.
//
// A wrapper over its wire rendering, distinct from `string` and from every other wrapper here for
// the reason the specification's own newtypes are distinct from their representations: a value's
// meaning is not its shape. The field is unexported, so the only way to make one is
// [NewDuration] — but Go's zero value needs no constructor, so `Duration{}` is still
// spellable (see TARGET.md).
type Duration struct {
	value string
}

// NewDuration wraps an ISO 8601 duration as a Duration.
func NewDuration(value string) Duration {
	return Duration{value: value}
}

// Value is the wrapped rendering.
func (v Duration) Value() string {
	return v.value
}

// Timestamp is an instant, carried as its wire rendering — RFC 3339, such as `2026-01-01T00:00:00Z`.
//
// A wrapper over its wire rendering, distinct from `string` and from every other wrapper here for
// the reason the specification's own newtypes are distinct from their representations: a value's
// meaning is not its shape. The field is unexported, so the only way to make one is
// [NewTimestamp] — but Go's zero value needs no constructor, so `Timestamp{}` is still
// spellable (see TARGET.md).
type Timestamp struct {
	value string
}

// NewTimestamp wraps an RFC 3339 instant as a Timestamp.
func NewTimestamp(value string) Timestamp {
	return Timestamp{value: value}
}

// Value is the wrapped rendering.
func (v Timestamp) Value() string {
	return v.value
}

// Uuid is a UUID, carried as its canonical textual rendering.
//
// A wrapper over its wire rendering, distinct from `string` and from every other wrapper here for
// the reason the specification's own newtypes are distinct from their representations: a value's
// meaning is not its shape. The field is unexported, so the only way to make one is
// [NewUuid] — but Go's zero value needs no constructor, so `Uuid{}` is still
// spellable (see TARGET.md).
type Uuid struct {
	value string
}

// NewUuid wraps a canonical UUID rendering as a Uuid.
func NewUuid(value string) Uuid {
	return Uuid{value: value}
}

// Value is the wrapped rendering.
func (v Uuid) Value() string {
	return v.value
}
