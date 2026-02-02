<style>
	/* Nested :where(:is()) - exceeds 100 chars, wraps with extra indent for complex selectors */
	:where(
		:is(
				p,
				ul,
				ol,
				menu,
				form,
				fieldset,
				table,
				details,
				textarea,
				select,
				label,
				pre,
				blockquote,
				aside,
				nav
			):not(:last-child):not(.unstyled)
	) {
		margin: 0;
	}
</style>
