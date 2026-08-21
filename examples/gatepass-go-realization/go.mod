// The hand-written half of the synthesised gatepass Go module: one implementation per obligation
// in `generated/go/gatepass/PLAN.md`, the linker that assembles them (gap register D-2), and the
// command that hands the assembled system to the generated HTTP surface.
//
// A module of its own rather than a package inside the generated tree, because that boundary is
// absolute: hand-written code satisfies generated interfaces by import, and the generated tree
// stays fully disposable.
//
// The `replace` below is a filesystem path, so `go build` here resolves nothing over a network —
// the same no-network property every other step of the gate holds. There is no `go.sum`, and
// there is nothing for one to check: this module has exactly one dependency and it is a directory.
module example.invalid/gatepass-realization

go 1.21

require example.invalid/gatepass v0.0.0

replace example.invalid/gatepass => ../../generated/go/gatepass
