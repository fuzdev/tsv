<script lang="ts">
	// Class member keys decode from escapes (property, method, get/set, static)
	class C {
		in: string;
		b() {}
		get c() {
			return 1;
		}
		set d(v) {}
		static e() {}
	}
</script>
