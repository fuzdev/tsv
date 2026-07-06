<script lang="ts">
	class C {
		// A static method named `constructor` is NOT the constructor: kind "method".
		static constructor() {}

		// The instance constructor: kind "constructor".
		constructor() {}
	}
</script>
