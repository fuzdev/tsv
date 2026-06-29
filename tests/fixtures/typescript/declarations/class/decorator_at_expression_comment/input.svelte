<script lang="ts">
	// inline block comment between @ and the decorator expression stays inline
	@/* a */ deca
	class A {}

	// own-line comment: trails @, expression on the next line
	@/* b */
	decb
	class B {}

	// member decorator: same rules
	class C {
		@/* c */ decc
		m() {}
	}
</script>
