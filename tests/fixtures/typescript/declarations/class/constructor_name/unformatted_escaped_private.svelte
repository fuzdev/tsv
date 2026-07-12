<script lang="ts">
	class C {
		// a `constructor` METHOD is the class constructor — valid
		constructor() {}
		// `static constructor` is a static method, not the constructor — valid
		static constructor() {}
		// a computed `['constructor']` key is a normal member, not the reserved name — valid
		['constructor'] = 1;
		// a normal private field (contrast with the illegal #constructor) — valid
		#\u0061 = 1;
	}
</script>
