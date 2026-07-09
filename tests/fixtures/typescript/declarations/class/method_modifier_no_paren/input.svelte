<script lang="ts">
	class C {
		// a get/set accessor requires a `(params)` list — a valid getter/setter pair
		get a() {
			return 1;
		}
		set a(v) {}
		// `async` and generator `*` are method-only modifiers — valid as methods
		async b() {}
		*c() {}
		// with no following member name, `get`/`set`/`async` are ordinary field names
		get = 1;
		async = 2;
	}
</script>
