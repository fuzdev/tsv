<script lang="ts">
	// Comment between accessor keyword and name is preserved
	class A {
		get /* c1 */ a() {
			return 1;
		}
		set /* c2 */ a(v) {}
	}
</script>
