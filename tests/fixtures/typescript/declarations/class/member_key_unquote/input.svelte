<script lang="ts">
	class C {
		foo() {}
		get y() {
			return 1;
		}
		static z() {}
		constructor() {}
		// A non-identifier method key stays quoted in both formatters.
		'0a'() {}
	}
</script>
