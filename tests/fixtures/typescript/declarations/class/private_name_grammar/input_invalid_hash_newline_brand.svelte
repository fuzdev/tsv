<script lang="ts">
	class C {
		#a;
		static check(x: object) {
			return #
				a in x;
		}
	}
</script>
