<script lang="ts">
	// 100 chars first line (prefix + ${identifier) — hugs, chain breaks at methods
	const c = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxx${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.map((s) => s.n).join(',')
	}`;

	// 101 chars first line (prefix + ${identifier) — hugs, chain breaks at methods
	const d = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.map((s) => s.n).join(',')
	}`;

	// 101 chars first line AND inner expression exceeds 100 chars — chain breaks at methods
	const e = `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			.map((s) => s.nnnnnnnnnnnnnnnn)
			.join(',')
	}`;
</script>
