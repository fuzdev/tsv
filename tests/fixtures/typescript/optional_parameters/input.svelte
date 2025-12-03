<script lang="ts">
	function opt(a?: number) {}
	function opt2(a: number, b?: string) {}
	function rest(...args: number[]) {}
	function rest2(a: number, ...rest: string[]) {}
	function def(a: number = 0) {}
	function def2(a: number, b: string = 'x') {}
	function def3(a?: number, b: string = 'y') {}
	function combined(a: number, b?: string, ...rest: boolean[]) {}
</script>
