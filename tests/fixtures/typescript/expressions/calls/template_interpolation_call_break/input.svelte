<script lang="ts">
	// Template with long interpolation — call breaks, expression stays flat.
	// Prettier renders expressions at infinite width, treating the template as atomic.
	parts.push(
		`prefix ${branch.gate_result.status} middle ${branch.gate_result.completed ?? 'pending'} suffix`
	);

	// Method chain callee
	log.info(
		`Item ${record.name} has status ${record.metadata.current_status} in ${record.metadata.cat}`
	);

	// Multi-arg — template stays flat, all args expand
	fn(
		first_arg,
		`template ${some.deeply.nested.property} and ${another.deeply.nested.prop} at the end`
	);
</script>
