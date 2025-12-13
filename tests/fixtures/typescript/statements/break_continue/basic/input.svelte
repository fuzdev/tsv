<script>
	outer: for (let i = 0; i < n; i++) {
		for (let j = 0; j < m; j++) {
			if (cond) break;
			if (cond) continue;
			if (cond) break outer;
			if (cond) continue outer;
		}
	}

	while (cond) {
		if (a) break;
		if (b) continue;
	}
</script>
