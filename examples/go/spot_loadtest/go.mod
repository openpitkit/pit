module openpit-loadtest-spot-funds-go

go 1.23.0

require (
	github.com/HdrHistogram/hdrhistogram-go v1.2.0
	github.com/shirou/gopsutil/v4 v4.25.4
	github.com/shopspring/decimal v1.4.0
	go.openpit.dev/openpit v0.5.0
	gopkg.in/ini.v1 v1.67.0
)

require (
	github.com/ebitengine/purego v0.8.2 // indirect
	github.com/go-ole/go-ole v1.3.0 // indirect
	github.com/lufia/plan9stats v0.0.0-20250317134145-8bc96cf8fc35 // indirect
	github.com/power-devops/perfstat v0.0.0-20240221224432-82ca36839d55 // indirect
	github.com/tklauser/go-sysconf v0.3.15 // indirect
	github.com/tklauser/numcpus v0.10.0 // indirect
	github.com/yusufpapurcu/wmi v1.2.4 // indirect
	golang.org/x/sys v0.33.0 // indirect
)

replace go.openpit.dev/openpit => ../../../bindings/go
