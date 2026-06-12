<script lang="ts">
	// Prettier PR #18325: Long class name without type params - extends breaks to new line
	// but member expression stays together (no break within a00000.b0000.c0000000)

	// 100 chars exactly - stays inline (at boundary)
	class ClassNameThatIsLongEnoughToTriggerWrapping00000000000000000 extends a00000.b0000.c0000000 {}

	// 101 chars - wraps extends to new line
	class ClassNameThatIsLongEnoughToTriggerWrapping000000000000000000
		extends a00000.b0000.c0000000 {}

	// Contrast: Simple identifier parent - NEVER wraps (Prettier quirk)
	class VeryLongClassNameThatDefinitelyExceedsOneHundredCharactersEasilyNoProblem extends ParentClass {}
</script>
