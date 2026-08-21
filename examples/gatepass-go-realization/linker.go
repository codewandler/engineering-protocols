package realization

import (
	"fmt"
	"strings"

	"example.invalid/gatepass/components/passservice"
	"example.invalid/gatepass/system"
)

// Obligations is every obligation the gatepass plan owes, as {capability, source} in the stubs'
// own spelling.
//
// Held equal to `generated/go/gatepass/plan.json` by TestTheLinkersObligationListIsExactlyThePlans,
// so a specification change that moves an obligation fails here instead of leaving the linker
// resolving a list that no longer exists.
var Obligations = [][2]string{
	{"command behaviour", "gatepass.visit.AdmitVisitor"},
	{"command behaviour", "gatepass.visit.RegisterVisit"},
	{"command behaviour", "gatepass.visit.SignOutVisitor"},
	{"view query", "gatepass.visit.ExpectedVisits"},
	{"view query", "gatepass.visit.VisitById"},
}

// Honest is how the honest offers name themselves in an ambiguity error.
const Honest = "gatepass-go-realization/honest"

// LinkError is why one obligation could not be resolved.
//
// Gap register D-2, taken as written: the linker does not choose. Zero implementations offered for
// an obligation is an unsatisfied obligation; two is an ambiguity naming both. There is
// deliberately no priority, no default and no "first wins".
type LinkError struct {
	// Capability is the capability kind, as the plan spells it.
	Capability string
	// Source is the construct that requires it, in the specification's own spelling.
	Source string
	// Offered is every claimant, in the order offered. Empty means nothing was offered.
	Offered []string
}

// Error renders the refusal.
func (e LinkError) Error() string {
	if len(e.Offered) == 0 {
		return fmt.Sprintf("nothing implements the %s `%s`, which the plan owes", e.Capability, e.Source)
	}
	return fmt.Sprintf(
		"%d implementations claim the %s `%s` — %s — and this linker does not choose between them",
		len(e.Offered), e.Capability, e.Source, strings.Join(e.Offered, ", "),
	)
}

// LinkErrors is every refusal one linking produced.
//
// Errors accumulate: a linker with three empty slots reports three unsatisfied obligations, not
// the first one it happened to walk.
type LinkErrors struct {
	// Errors is the refusals, in the order the obligations are listed.
	Errors []LinkError
}

// Error renders every refusal, one per line.
func (e LinkErrors) Error() string {
	lines := make([]string, 0, len(e.Errors))
	for _, held := range e.Errors {
		lines = append(lines, held.Error())
	}
	return strings.Join(lines, "\n")
}

// Offers is what is on offer for each obligation, before anything is resolved.
type Offers struct {
	claims map[[2]string][]string
}

// NewOffers is an empty offer sheet.
func NewOffers() *Offers {
	return &Offers{claims: map[[2]string][]string{}}
}

// Offer records one claim.
func (o *Offers) Offer(capability string, source string, by string) {
	key := [2]string{capability, source}
	known := false
	for _, owed := range Obligations {
		if owed == key {
			known = true
			break
		}
	}
	if !known {
		panic(fmt.Sprintf("`%s` `%s` is not an obligation this plan owes; the linker's list and the plan have diverged", capability, source))
	}
	o.claims[key] = append(o.claims[key], by)
}

// Resolve resolves every obligation, accumulating what could not be resolved.
func (o *Offers) Resolve() error {
	var errors []LinkError
	for _, owed := range Obligations {
		claimants := o.claims[owed]
		if len(claimants) == 1 {
			continue
		}
		errors = append(errors, LinkError{Capability: owed[0], Source: owed[1], Offered: claimants})
	}
	if len(errors) == 0 {
		return nil
	}
	return LinkErrors{Errors: errors}
}

// Assembled is everything one linking produced: the system, and the store it answers over.
type Assembled struct {
	// System is what every command enters through.
	System *system.System
	// Store is what the realization answers over, for a caller that wants to look.
	Store *Store
}

// Link is the honest linkage: exactly one implementation per obligation, resolved rather than
// assumed.
//
// The resolution runs even though it cannot fail for the offers made here, which is the point: a
// mechanism only exercised when it fails is a mechanism nobody has run.
func Link() (*Assembled, error) {
	store := NewStore()
	realization := Over(store)

	offers := NewOffers()
	for _, owed := range Obligations {
		offers.Offer(owed[0], owed[1], Honest)
	}
	if err := offers.Resolve(); err != nil {
		return nil, err
	}
	return &Assembled{
		System: system.NewSystem(passservice.New(realization)),
		Store:  store,
	}, nil
}
