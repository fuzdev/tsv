<script lang="ts">
	// Without `!`, both formatters hang the outer parens (consistent — see
	// `member/paren_base_trailing_long/`).
	const a = (
		await call(
			applicationObjectLong,
			specificationObjectLong,
			{aaa: parentNodeId, bbb: childNodeId},
			hh,
		)
	).ok;

	// With `!`, tsv still hangs the outer parens, but Prettier hugs the inner call
	// (`(await call(\n...\n))!.ok`) — the same base laid out two different ways.
	const b = (await call(
		applicationObjectLong,
		specificationObjectLong,
		{aaa: parentNodeId, bbb: childNodeId},
		hh,
	))!.ok;
</script>
