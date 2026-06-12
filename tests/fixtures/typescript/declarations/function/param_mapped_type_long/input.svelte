<script lang="ts">
	// 100 chars - stays inline
	function fn1(a?: {[K in keyof TAAAAAAAAAAAAAAAAAAAAAAAAX]: TAAAAAAAAAAAAAAAAAAAAAAAA[K]}): void {}

	// 101 chars - wraps: a?: { hugs function name, mapped type body expands
	function fn2(a?: {
		[K in keyof TAAAAAAAAAAAAAAAAAAAAAAAAXB]: TAAAAAAAAAAAAAAAAAAAAAAAA[K];
	}): void {}
</script>
