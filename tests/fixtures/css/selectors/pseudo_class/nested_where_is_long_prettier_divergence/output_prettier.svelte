<style>
	/* Nested :where(:is()) exceeds 100 chars and wraps; the :is() args indent one
	   level (uniform), not the extra level prettier adds for a >2-node compound */
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
