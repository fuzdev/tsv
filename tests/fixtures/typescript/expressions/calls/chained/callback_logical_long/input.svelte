<script lang="ts">
	// 100 chars on line (at boundary)
	const a = items
		.filter(
			aa && bb && cc && dd && ee && ff && gg && hh && ii && jj && kk && ll && mm && nn && oo && pppp
		)
		.join();

	// 101 chars - exceeds, so wraps
	const b = items
		.filter(
			aa &&
				bb &&
				cc &&
				dd &&
				ee &&
				ff &&
				gg &&
				hh &&
				ii &&
				jj &&
				kk &&
				ll &&
				mm &&
				nn &&
				oo &&
				ppppp
		)
		.join();
</script>
