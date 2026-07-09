<script lang="ts">
	class C {
		// A `static` accessor named `constructor` is not the constructor — valid.
		static get constructor() {}
		static set constructor(v) {}
		// A computed `['constructor']` key is a normal accessor, not the constructor — valid.
		get ['constructor']() {}
	}
</script>
