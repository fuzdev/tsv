<script lang="ts">
	// short — arrow body fits inline after =>
	const a = (t: T): string => read(get(t), Out).a.b;

	// 100 — the base call plus its trailing members all fit on the arrow body line
	const b = (t: T): string =>
		read(get(t, segmentValue, paramAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa), Out).a.b;

	// 101 — one char longer: only the last member `.b` drops; the call args do NOT break
	const c = (t: T): string =>
		read(get(t, segmentValue, paramAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa), Out).a
			.b;
</script>
