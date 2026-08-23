package main

import rego.v1

deny contains msg if {
	some name, job in input.jobs
	some step in job.steps
	run := step.run
	is_string(run)
	some line in split(run, "\n")
	regex.match(`(^|[[:space:]])mise[[:space:]].*\brun[[:space:]].*\bver[[:space:]]+--`, line)
	msg := sprintf("%s: mise run ver -- is mise's separator; use mise run ver <verb>", [name])
}
