<style>
	div {
		/* 100 chars - stays inline */
		--ssssss: var(--a) color-mix(in hsl, var(--b) var(--c, var(--d00000000000000000)), transparent);

		/* 102 chars - wrap between values, keep color-mix inline */
		--ssssss: var(--a)
			color-mix(in hsl, var(--b) var(--c, var(--d0000000000000000000)), transparent);

		/* Corpus case: long property name with nested vars (149 chars total) */
		--tab_hover_ssssss: var(--ssssss_inset_bottom_xs)
			color-mix(in hsl, var(--ssssss_sssss) var(--ssssss_sssss, var(--ssssss_sssss_1)), transparent);
	}
</style>
