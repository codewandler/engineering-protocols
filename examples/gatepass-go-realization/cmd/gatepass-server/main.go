// Command gatepass-server is the gatepass system, realized, on the wire.
//
// Thirty lines, and the arrow points the way it always points here: this command links the
// hand-written realization into the generated surface, and the generated surface knows nothing
// about it. server.ServePassService binds, writes the startup record and answers the routes the
// committed OpenAPI document declares; everything it answers *with* comes through the port from
// this module's linker.
//
// # The port comes from the environment, not from an argument
//
// PORT unset or 0 binds an ephemeral port, which is what makes the gate's demonstration
// deterministic: two of these run side by side without agreeing about a number in advance, and each
// says in its startup record which port it took. There is no flag parsing here at all — a
// synthesised surface takes no options, so there is nothing to parse.
package main

import (
	"fmt"
	"os"

	realization "example.invalid/gatepass-realization"
	"example.invalid/gatepass/server"
)

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "0"
	}
	assembled, err := realization.Link()
	if err != nil {
		fmt.Fprintf(os.Stderr, "{\"log\":\"ess/1\",\"event\":\"system.unlinked\",\"reason\":%q}\n", err.Error())
		os.Exit(1)
	}
	if err := server.ServePassService(assembled.System, "127.0.0.1:"+port); err != nil {
		fmt.Fprintf(os.Stderr, "{\"log\":\"ess/1\",\"event\":\"system.stopped\",\"reason\":%q}\n", err.Error())
		os.Exit(1)
	}
}
