# Every mise version a workflow names as a literal equals min_version in
# mise.toml, so a pin bump that misses one copy fails here instead of in a
# release run. Run with conftest --combine over the workflows and mise.toml.
package mise_pins

import rego.v1

min_version := v if {
	some f in input
	f.path == "mise.toml"
	v := f.contents.min_version
}

workflows contains f if {
	some f in input
	startswith(f.path, ".github/workflows/")
}

literal(v) if {
	is_string(v)
	not contains(v, "${{")
}

# The YAML parser reads the `on` key as the boolean true.
trigger(doc) := object.get(doc, "on", object.get(doc, "true", {}))

deny contains msg if {
	some f in workflows
	some name, job in f.contents.jobs
	some step in job.steps
	startswith(step.uses, "jdx/mise-action@")
	v := step["with"].version
	literal(v)
	v != min_version
	msg := sprintf("%s: job %s installs mise %s, but mise.toml min_version is %s", [f.path, name, v, min_version])
}

deny contains msg if {
	some f in workflows
	some name, job in f.contents.jobs
	v := job["with"]["mise-version"]
	literal(v)
	v != min_version
	msg := sprintf("%s: job %s passes mise-version %s, but mise.toml min_version is %s", [f.path, name, v, min_version])
}

deny contains msg if {
	some f in workflows
	v := trigger(f.contents).workflow_call.inputs["mise-version"]["default"]
	literal(v)
	v != min_version
	msg := sprintf("%s: the mise-version input defaults to %s, but mise.toml min_version is %s", [f.path, v, min_version])
}
