<script lang="ts">
	// Prettier PR #18325: Long class name without type params - extends breaks to new line
	// but member expression stays together (no break within eslint.Rule.RuleModule)
	class ThisIsAnExtremelyLongClassNameThatWillDefinitelyExceedThePrintWidth
		extends eslint.Rule.RuleModule {}

	// Contrast: Short class stays inline
	class Short extends foo.bar.baz {}
</script>
