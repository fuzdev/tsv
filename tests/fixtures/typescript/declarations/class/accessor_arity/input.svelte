<script lang="ts">
	class C {
		// getter with no params, setter with exactly one param — valid
		get a() {}
		set a(x) {}
		// the one setter param may be typed or destructuring — valid
		set b(x: number) {}
		set c([x]) {}
		// optional / default setter params parse (acorn accepts; tsc TS1051/TS1052 is a
		// checker rule deferred to diagnostics)
		set d(x?) {}
		set e(x = 1) {}
	}
</script>
