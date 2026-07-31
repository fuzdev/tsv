<script lang="ts">
	class C {
		#a = 1;
		fn() {
			return this.#/* c */ a;
		}
	}
</script>
