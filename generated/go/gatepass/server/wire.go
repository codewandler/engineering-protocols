// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

package server

import (
	"encoding/base64"
	"encoding/json"
	"example.invalid/gatepass/types/primitives"
	"example.invalid/gatepass/types/visit"
	"fmt"
	"strconv"
)

// DecodeError is a refusal at one path, with what the declaration says belongs there and what
// arrived instead.
//
// The path is what makes it usable: a caller that sent a nested command input gets the field, not
// "invalid request".
type DecodeError struct {
	// At is where in the document, as a dotted path from its root.
	At string
	// Expected is what the declaration says belongs there.
	Expected string
	// Found is what was there instead.
	Found string
}

// Error renders the refusal.
func (e DecodeError) Error() string {
	return fmt.Sprintf("%s: expected %s, found %s", e.At, e.Expected, e.Found)
}

// describes names what a decoded JSON value is, for a refusal.
func describes(value any) string {
	switch shaped := value.(type) {
	case nil:
		return "null"
	case bool:
		return "a boolean"
	case json.Number:
		return "a number"
	case string:
		return "a string"
	case []any:
		return "an array"
	case map[string]any:
		return "an object"
	default:
		_ = shaped
		return "a value of an unknown shape"
	}
}

// nested is one step further into a document, for a message a reader can follow back.
func nested(at string, step string) string {
	if at == "" {
		return step
	}
	return at + "." + step
}

// indexed is one step into an array.
func indexed(at string, index int) string {
	return fmt.Sprintf("%s[%d]", at, index)
}

// objectAt is the object at this path.
func objectAt(value any, at string, expected string) (map[string]any, error) {
	object, ok := value.(map[string]any)
	if !ok {
		return nil, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return object, nil
}

// itemsAt is the array at this path.
func itemsAt(value any, at string, expected string) ([]any, error) {
	items, ok := value.([]any)
	if !ok {
		return nil, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return items, nil
}

// required is the member a declaration says must be there, and the path it sits at.
func required(value any, at string, name string) (any, string, error) {
	memberAt := nested(at, name)
	object, err := objectAt(value, at, "an object")
	if err != nil {
		return nil, memberAt, err
	}
	member, ok := object[name]
	if !ok {
		// The same sentence the Rust target's reader writes, word for word. Two applications
		// synthesised from one specification and refusing one request differently would be two
		// diagnostics a caller has to learn, and `cargo xtask synth --check` compares the bodies.
		return nil, memberAt, DecodeError{At: memberAt, Expected: "a value", Found: "nothing"}
	}
	return member, memberAt, nil
}

// optional is the member a declaration says may be there. An absent member and a null one are the
// same answer, because the published contract omits an absent optional rather than sending null.
func optional(value any, at string, name string) (any, string, bool, error) {
	memberAt := nested(at, name)
	object, err := objectAt(value, at, "an object")
	if err != nil {
		return nil, memberAt, false, err
	}
	member, ok := object[name]
	if !ok || member == nil {
		return nil, memberAt, false, nil
	}
	return member, memberAt, true, nil
}

// textAt is the string at this path.
func textAt(value any, at string, expected string) (string, error) {
	text, ok := value.(string)
	if !ok {
		return "", DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return text, nil
}

// boolAt is the boolean at this path.
func boolAt(value any, at string, expected string) (bool, error) {
	held, ok := value.(bool)
	if !ok {
		return false, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return held, nil
}

// integerAt is the whole number at this path.
//
// Read through json.Number, which is why the decoder is configured with UseNumber: the default
// float64 loses whole numbers past 2^53, and an Integer in this model is 64 bits.
func integerAt(value any, at string, expected string) (int64, error) {
	number, ok := value.(json.Number)
	if !ok {
		return 0, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	held, err := number.Int64()
	if err != nil {
		return 0, DecodeError{At: at, Expected: expected, Found: fmt.Sprintf("`%s`", number.String())}
	}
	return held, nil
}

// bytesAt is the base64-encoded bytes at this path.
func bytesAt(value any, at string, expected string) ([]byte, error) {
	text, err := textAt(value, at, expected)
	if err != nil {
		return nil, err
	}
	held, decodeErr := base64.StdEncoding.DecodeString(text)
	if decodeErr != nil {
		return nil, DecodeError{At: at, Expected: expected, Found: fmt.Sprintf("`%s`", text)}
	}
	return held, nil
}

// keyBool reads a boolean written as an object key.
func keyBool(key string, at string) (bool, error) {
	held, err := strconv.ParseBool(key)
	if err != nil {
		return false, DecodeError{At: at, Expected: "a key spelling true or false", Found: fmt.Sprintf("`%s`", key)}
	}
	return held, nil
}

// keyInteger reads a whole number written as an object key.
func keyInteger(key string, at string) (int64, error) {
	held, err := strconv.ParseInt(key, 10, 64)
	if err != nil {
		return 0, DecodeError{At: at, Expected: "a key spelling a whole number", Found: fmt.Sprintf("`%s`", key)}
	}
	return held, nil
}

// keyBytes reads base64-encoded bytes written as an object key.
func keyBytes(key string, at string) ([]byte, error) {
	held, err := base64.StdEncoding.DecodeString(key)
	if err != nil {
		return nil, DecodeError{At: at, Expected: "a key spelling base64-encoded bytes", Found: fmt.Sprintf("`%s`", key)}
	}
	return held, nil
}

// encodeGatepassVisitBadge writes `gatepass.visit.Badge` as JSON.
func encodeGatepassVisitBadge(value visit.Badge) any {
	out := map[string]any{}
	out["serial"] = value.Serial
	if value.PrintedAt != nil {
		held0 := *value.PrintedAt
		out["printed_at"] = held0.Value()
	}
	out["signature"] = base64.StdEncoding.EncodeToString(value.Signature)
	return out
}

// decodeGatepassVisitBadge reads `gatepass.visit.Badge` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitBadge(value any, at string) (visit.Badge, error) {
	var out visit.Badge
	if _, err := objectAt(value, at, "an object"); err != nil {
		return out, err
	}
	member0, at0, err := required(value, at, "serial")
	if err != nil {
		return out, err
	}
	held1, err := textAt(member0, at0, "a string")
	if err != nil {
		return out, err
	}
	out.Serial = held1
	member2, at2, found2, err := optional(value, at, "printed_at")
	if err != nil {
		return out, err
	}
	if found2 {
		held3, err := textAt(member2, at2, "an RFC 3339 timestamp as a string")
		if err != nil {
			return out, err
		}
		some2 := primitives.NewTimestamp(held3)
		out.PrintedAt = &some2
	}
	member4, at4, err := required(value, at, "signature")
	if err != nil {
		return out, err
	}
	held5, err := bytesAt(member4, at4, "base64-encoded bytes")
	if err != nil {
		return out, err
	}
	out.Signature = held5
	return out, nil
}

// encodeGatepassVisitBuilding writes `gatepass.visit.Building` as JSON.
func encodeGatepassVisitBuilding(value visit.Building) any {
	switch value.(type) {
	case visit.BuildingNorth:
		return "North"
	case visit.BuildingSouth:
		return "South"
	case visit.BuildingAnnex:
		return "Annex"
	default:
		// Go cannot check that a switch over a sealed interface is total (see TARGET.md).
		// A value no branch above names is one no generated code can construct.
		return nil
	}
}

// decodeGatepassVisitBuilding reads `gatepass.visit.Building` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitBuilding(value any, at string) (visit.Building, error) {
	var out visit.Building
	text, err := textAt(value, at, "one of `North`, `South`, `Annex`")
	if err != nil {
		return out, err
	}
	switch text {
	case "North":
		return visit.BuildingNorth{}, nil
	case "South":
		return visit.BuildingSouth{}, nil
	case "Annex":
		return visit.BuildingAnnex{}, nil
	}
	return out, DecodeError{At: at, Expected: "one of `North`, `South`, `Annex`", Found: fmt.Sprintf("`%s`", text)}
}

// encodeGatepassVisitDeposit writes `gatepass.visit.Deposit` as JSON.
func encodeGatepassVisitDeposit(value visit.Deposit) any {
	out := map[string]any{}
	out["amount"] = value.Amount.Value()
	out["currency"] = value.Currency
	return out
}

// decodeGatepassVisitDeposit reads `gatepass.visit.Deposit` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitDeposit(value any, at string) (visit.Deposit, error) {
	var out visit.Deposit
	if _, err := objectAt(value, at, "an object"); err != nil {
		return out, err
	}
	member0, at0, err := required(value, at, "amount")
	if err != nil {
		return out, err
	}
	held1, err := textAt(member0, at0, "a decimal as a string, such as `10.50`")
	if err != nil {
		return out, err
	}
	out.Amount = primitives.NewDecimal(held1)
	member2, at2, err := required(value, at, "currency")
	if err != nil {
		return out, err
	}
	held3, err := textAt(member2, at2, "a string")
	if err != nil {
		return out, err
	}
	out.Currency = held3
	return out, nil
}

// encodeGatepassVisitEmployeeId writes `gatepass.visit.EmployeeId` as JSON.
func encodeGatepassVisitEmployeeId(value visit.EmployeeId) any {
	return value.Value()
}

// decodeGatepassVisitEmployeeId reads `gatepass.visit.EmployeeId` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitEmployeeId(value any, at string) (visit.EmployeeId, error) {
	var out visit.EmployeeId
	held0, err := textAt(value, at, "a string")
	if err != nil {
		return out, err
	}
	return visit.NewEmployeeId(held0), nil
}

// encodeGatepassVisitHost writes `gatepass.visit.Host` as JSON.
func encodeGatepassVisitHost(value visit.Host) any {
	switch shape := value.(type) {
	case visit.HostContractor:
		out := map[string]any{}
		out["kind"] = "contractor"
		out["value"] = encodeGatepassVisitVendorRef(shape.Value)
		return out
	case visit.HostEmployee:
		out := map[string]any{}
		out["kind"] = "employee"
		out["value"] = encodeGatepassVisitEmployeeId(shape.Value)
		return out
	default:
		_ = shape
		// Go cannot check that a switch over a sealed interface is total (see TARGET.md).
		// A shape no branch above names is one no generated code can construct.
		return nil
	}
}

// decodeGatepassVisitHost reads `gatepass.visit.Host` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitHost(value any, at string) (visit.Host, error) {
	var out visit.Host
	tagged, tagAt, err := required(value, at, "kind")
	if err != nil {
		return out, err
	}
	label, err := textAt(tagged, tagAt, "one of `contractor`, `employee`")
	if err != nil {
		return out, err
	}
	switch label {
	case "contractor":
		member0, at0, err := required(value, at, "value")
		if err != nil {
			return out, err
		}
		held1, err := decodeGatepassVisitVendorRef(member0, at0)
		if err != nil {
			return out, err
		}
		shape := held1
		return visit.HostContractor{Value: shape}, nil
	case "employee":
		member0, at0, err := required(value, at, "value")
		if err != nil {
			return out, err
		}
		held1, err := decodeGatepassVisitEmployeeId(member0, at0)
		if err != nil {
			return out, err
		}
		shape := held1
		return visit.HostEmployee{Value: shape}, nil
	}
	return out, DecodeError{At: tagAt, Expected: "one of `contractor`, `employee`", Found: fmt.Sprintf("`%s`", label)}
}

// encodeGatepassVisitVendorRef writes `gatepass.visit.VendorRef` as JSON.
func encodeGatepassVisitVendorRef(value visit.VendorRef) any {
	return value.Value()
}

// decodeGatepassVisitVendorRef reads `gatepass.visit.VendorRef` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitVendorRef(value any, at string) (visit.VendorRef, error) {
	var out visit.VendorRef
	held0, err := textAt(value, at, "a string")
	if err != nil {
		return out, err
	}
	return visit.NewVendorRef(held0), nil
}

// encodeGatepassVisitVisitState writes `gatepass.visit.Visit.State` as JSON.
func encodeGatepassVisitVisitState(value visit.VisitState) any {
	switch value.(type) {
	case visit.VisitStateDeparted:
		return "Departed"
	case visit.VisitStateExpected:
		return "Expected"
	case visit.VisitStateOnSite:
		return "OnSite"
	default:
		// Go cannot check that a switch over a sealed interface is total (see TARGET.md).
		// A value no branch above names is one no generated code can construct.
		return nil
	}
}

// decodeGatepassVisitVisitState reads `gatepass.visit.Visit.State` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitVisitState(value any, at string) (visit.VisitState, error) {
	var out visit.VisitState
	text, err := textAt(value, at, "one of `Departed`, `Expected`, `OnSite`")
	if err != nil {
		return out, err
	}
	switch text {
	case "Departed":
		return visit.VisitStateDeparted{}, nil
	case "Expected":
		return visit.VisitStateExpected{}, nil
	case "OnSite":
		return visit.VisitStateOnSite{}, nil
	}
	return out, DecodeError{At: at, Expected: "one of `Departed`, `Expected`, `OnSite`", Found: fmt.Sprintf("`%s`", text)}
}

// encodeGatepassVisitVisitId writes `gatepass.visit.VisitId` as JSON.
func encodeGatepassVisitVisitId(value visit.VisitId) any {
	return value.Value().Value()
}

// decodeGatepassVisitVisitId reads `gatepass.visit.VisitId` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitVisitId(value any, at string) (visit.VisitId, error) {
	var out visit.VisitId
	held0, err := textAt(value, at, "a UUID as a string")
	if err != nil {
		return out, err
	}
	return visit.NewVisitId(primitives.NewUuid(held0)), nil
}

// encodeGatepassVisitVisitorName writes `gatepass.visit.VisitorName` as JSON.
func encodeGatepassVisitVisitorName(value visit.VisitorName) any {
	return value.Value()
}

// decodeGatepassVisitVisitorName reads `gatepass.visit.VisitorName` from JSON, or refuses at the path it was reached at.
func decodeGatepassVisitVisitorName(value any, at string) (visit.VisitorName, error) {
	var out visit.VisitorName
	held0, err := textAt(value, at, "a string")
	if err != nil {
		return out, err
	}
	return visit.NewVisitorName(held0), nil
}

// encodeErrorGatepassVisitInvalidVisitLength writes the declared error `gatepass.visit.InvalidVisitLength` as JSON.
func encodeErrorGatepassVisitInvalidVisitLength(value visit.InvalidVisitLength) any {
	out := map[string]any{}
	out["submitted"] = value.Submitted
	return out
}

// encodeErrorGatepassVisitVisitStateConflict writes the declared error `gatepass.visit.VisitStateConflict` as JSON.
func encodeErrorGatepassVisitVisitStateConflict(value visit.VisitStateConflict) any {
	out := map[string]any{}
	out["state"] = encodeGatepassVisitVisitState(value.State)
	return out
}

// encodeViewGatepassVisitExpectedVisits writes one row of the view `gatepass.visit.ExpectedVisits` as JSON.
func encodeViewGatepassVisitExpectedVisits(value visit.ExpectedVisits) any {
	out := map[string]any{}
	out["visit_id"] = encodeGatepassVisitVisitId(value.VisitId)
	out["visitor"] = encodeGatepassVisitVisitorName(value.Visitor)
	out["building"] = encodeGatepassVisitBuilding(value.Building)
	out["deposit"] = encodeGatepassVisitDeposit(value.Deposit)
	return out
}

// encodeViewGatepassVisitVisitById writes one row of the view `gatepass.visit.VisitById` as JSON.
func encodeViewGatepassVisitVisitById(value visit.VisitById) any {
	out := map[string]any{}
	out["visit_id"] = encodeGatepassVisitVisitId(value.VisitId)
	out["visitor"] = encodeGatepassVisitVisitorName(value.Visitor)
	out["host"] = encodeGatepassVisitHost(value.Host)
	items0 := make([]any, 0, len(value.Escorts))
	for _, element := range value.Escorts {
		items0 = append(items0, encodeGatepassVisitVisitorName(element))
	}
	out["escorts"] = items0
	entries1 := map[string]any{}
	for key, element := range value.Notes {
		entries1[key] = element
	}
	out["notes"] = entries1
	if value.Badge != nil {
		held2 := *value.Badge
		out["badge"] = encodeGatepassVisitBadge(held2)
	}
	return out
}

// decodeCommandGatepassVisitAdmitVisitor reads the input of `gatepass.visit.AdmitVisitor` from JSON.
func decodeCommandGatepassVisitAdmitVisitor(value any, at string) (visit.AdmitVisitor, error) {
	var out visit.AdmitVisitor
	if _, err := objectAt(value, at, "an object"); err != nil {
		return out, err
	}
	member0, at0, err := required(value, at, "visit_id")
	if err != nil {
		return out, err
	}
	held1, err := decodeGatepassVisitVisitId(member0, at0)
	if err != nil {
		return out, err
	}
	out.VisitId = held1
	member2, at2, err := required(value, at, "badge")
	if err != nil {
		return out, err
	}
	held3, err := decodeGatepassVisitBadge(member2, at2)
	if err != nil {
		return out, err
	}
	out.Badge = held3
	return out, nil
}

// decodeCommandGatepassVisitRegisterVisit reads the input of `gatepass.visit.RegisterVisit` from JSON.
func decodeCommandGatepassVisitRegisterVisit(value any, at string) (visit.RegisterVisit, error) {
	var out visit.RegisterVisit
	if _, err := objectAt(value, at, "an object"); err != nil {
		return out, err
	}
	member0, at0, err := required(value, at, "visitor")
	if err != nil {
		return out, err
	}
	held1, err := decodeGatepassVisitVisitorName(member0, at0)
	if err != nil {
		return out, err
	}
	out.Visitor = held1
	member2, at2, err := required(value, at, "building")
	if err != nil {
		return out, err
	}
	held3, err := decodeGatepassVisitBuilding(member2, at2)
	if err != nil {
		return out, err
	}
	out.Building = held3
	member4, at4, err := required(value, at, "host")
	if err != nil {
		return out, err
	}
	held5, err := decodeGatepassVisitHost(member4, at4)
	if err != nil {
		return out, err
	}
	out.Host = held5
	member6, at6, err := required(value, at, "expected_minutes")
	if err != nil {
		return out, err
	}
	held7, err := integerAt(member6, at6, "a whole number")
	if err != nil {
		return out, err
	}
	out.ExpectedMinutes = held7
	member8, at8, err := required(value, at, "expected_stay")
	if err != nil {
		return out, err
	}
	held9, err := textAt(member8, at8, "an ISO 8601 duration as a string, such as `P30D`")
	if err != nil {
		return out, err
	}
	out.ExpectedStay = primitives.NewDuration(held9)
	member10, at10, err := required(value, at, "deposit")
	if err != nil {
		return out, err
	}
	held11, err := decodeGatepassVisitDeposit(member10, at10)
	if err != nil {
		return out, err
	}
	out.Deposit = held11
	member12, at12, err := required(value, at, "escorts")
	if err != nil {
		return out, err
	}
	items13, err := itemsAt(member12, at12, "an array")
	if err != nil {
		return out, err
	}
	held13 := make([]visit.VisitorName, 0, len(items13))
	for index, element := range items13 {
		elementAt := indexed(at12, index)
		held14, err := decodeGatepassVisitVisitorName(element, elementAt)
		if err != nil {
			return out, err
		}
		held13 = append(held13, held14)
	}
	out.Escorts = held13
	member15, at15, err := required(value, at, "notes")
	if err != nil {
		return out, err
	}
	entries16, err := objectAt(member15, at15, "an object")
	if err != nil {
		return out, err
	}
	held16 := make(map[string]string, len(entries16))
	for key, element := range entries16 {
		entryAt := nested(at15, key)
		held18, err := textAt(element, entryAt, "a string")
		if err != nil {
			return out, err
		}
		held16[key] = held18
	}
	out.Notes = held16
	member19, at19, err := required(value, at, "on_watchlist")
	if err != nil {
		return out, err
	}
	held20, err := boolAt(member19, at19, "true or false")
	if err != nil {
		return out, err
	}
	out.OnWatchlist = held20
	return out, nil
}

// decodeCommandGatepassVisitSignOutVisitor reads the input of `gatepass.visit.SignOutVisitor` from JSON.
func decodeCommandGatepassVisitSignOutVisitor(value any, at string) (visit.SignOutVisitor, error) {
	var out visit.SignOutVisitor
	if _, err := objectAt(value, at, "an object"); err != nil {
		return out, err
	}
	member0, at0, err := required(value, at, "visit_id")
	if err != nil {
		return out, err
	}
	held1, err := decodeGatepassVisitVisitId(member0, at0)
	if err != nil {
		return out, err
	}
	out.VisitId = held1
	return out, nil
}
