<script lang="ts">
	class C {
		// a plain constructor is neither async nor a generator
		constructor() {}
		// an async method (not the constructor) is fine
		async m() {}
		// a generator method (not the constructor) is fine
		*g() {}
	}
</script>
