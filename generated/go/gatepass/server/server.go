// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package server is the HTTP surface of every component the specification says is reached
// over a network.
//
// The codecs beside this file are generated rather than derived: a generated type carries
// an unexported field, which `encoding/json` cannot see, and exporting it would undo the
// distinctness the newtype encoding exists for. What they render is what the published
// contracts already fix — bytes as base64, a decimal, timestamp, duration and UUID as
// strings, an absent optional member omitted rather than sent as null.
package server

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
)

// The media type every answer derived from the model carries.
const mediaJSON = "application/json"

// The media type the prose answer carries.
//
// The bytes served are the committed Markdown, unrendered: rendering it to HTML here would be a
// second rendering of the documentation, and the two would differ the first time either moved.
const mediaMarkdown = "text/markdown; charset=utf-8"

// The largest body this surface reads, in bytes.
//
// A caller can claim any length, and a server that allocated whatever it was told to is a server
// anyone can stop by saying a large number. A megabyte is far past any command input this model
// can describe.
const maxBody = 1048576

// response is one answer: a status, a media type and a body.
type response struct {
	status      int
	contentType string
	body        string
}

// write sends the answer and lets the connection close behind it.
//
// Content-Length is set rather than left to the server: without it a body past the write buffer is
// sent with chunked transfer encoding, and the two applications synthesised from one specification
// would then differ on the wire for a reason no reader of the specification could predict. A caller
// that reads to the end of the connection gets the same bytes from both.
func (r response) write(writer http.ResponseWriter) {
	writer.Header().Set("Content-Type", r.contentType)
	writer.Header().Set("Content-Length", strconv.Itoa(len(r.body)))
	writer.Header().Set("Connection", "close")
	writer.WriteHeader(r.status)
	_, _ = writer.Write([]byte(r.body))
}

// refusal is an answer this surface makes rather than the specification.
//
// A malformed request, a path nothing declares, a method a path does not answer, an obligation
// nothing has satisfied. None of these is a declared outcome and none is published in the
// contract, because each is a fact about a transport rather than about a command. The body is
// JSON with one member: a caller that has just failed to satisfy a contract should not have to
// parse a second format to read why.
func refusal(status int, detail string) response {
	return rendered(status, map[string]any{"refused": detail})
}

// methodNotAllowed is the answer for a path this surface holds under a different method.
func methodNotAllowed(allowed string) response {
	return refusal(405, fmt.Sprintf("this path answers `%s`, and the contract declares no other method for it", allowed))
}

// rendered is one answer whose body is a value this package built.
func rendered(status int, body any) response {
	encoded, err := json.Marshal(body)
	if err != nil {
		return response{status: 500, contentType: mediaJSON, body: `{"refused":"the answer could not be encoded"}`}
	}
	return response{status: status, contentType: mediaJSON, body: string(encoded)}
}

// readBody reads at most maxBody bytes of a request, or the refusal that says why it could not.
func readBody(request *http.Request) ([]byte, *response) {
	if request.Body == nil {
		return nil, nil
	}
	defer func() { _ = request.Body.Close() }()
	body, err := io.ReadAll(io.LimitReader(request.Body, maxBody+1))
	if err != nil {
		answer := refusal(400, fmt.Sprintf("the body could not be read: %s", err))
		return nil, &answer
	}
	if len(body) > maxBody {
		answer := refusal(413, fmt.Sprintf("the body is longer than %d bytes, which is all this surface reads", maxBody))
		return nil, &answer
	}
	return body, nil
}

// readJSON parses a request body, or the refusal that says why it is not JSON.
//
// UseNumber, so an Integer past 2^53 survives the crossing: the default reads every number as a
// float64, and a visit id or a count would come back changed.
func readJSON(body []byte) (any, *response) {
	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.UseNumber()
	var value any
	if err := decoder.Decode(&value); err != nil {
		answer := refusal(400, fmt.Sprintf("the body is not JSON: %s", err))
		return nil, &answer
	}
	return value, nil
}
