package main

import rego.v1

deny contains msg if {
	some name, job in input.jobs
	some step in job.steps
	run := step.run
	is_string(run)
	line := trim_space(split(run, "\n")[0])
	startswith(line, "mise run ver --")
	msg := sprintf("%s: mise run ver -- is mise's separator; use mise run ver <verb>", [name])
}
