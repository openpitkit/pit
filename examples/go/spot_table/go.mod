module openpit-example-spot-table

go 1.22

require go.openpit.dev/openpit v0.3.0

require github.com/shopspring/decimal v1.4.0 // indirect

// Local development: build against monorepo sources. The release-e2e flow
// drops this directive and pins the require above to the published version
// so the example exercises exactly what an SDK consumer sees.
replace go.openpit.dev/openpit => ../../../bindings/go
