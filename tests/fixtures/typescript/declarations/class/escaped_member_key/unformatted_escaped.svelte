<script lang="ts">
	// Class member keys decode from escapes (property, method, get/set, static)
	class C {
		\u0069n: string;
		\u0062() {}
		get \u0063() {
			return 1;
		}
		set \u0064(v) {}
		static \u0065() {}
	}
</script>
