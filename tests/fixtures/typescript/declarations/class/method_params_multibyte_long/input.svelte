<script lang="ts">
	class C {
		// multibyte params, visually 80 cols — stays inline
		m(a: number, b: 名名名名名名名名名名名名名名名名名名名名名名名名名): void {}

		// multibyte params at exactly 100 cols — stays inline
		method(aa: number, bb: 名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名A): void {}

		// multibyte params at exactly 101 cols — wraps
		method(
			aa: number,
			bb: 名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名,
		): void {}
	}
</script>
