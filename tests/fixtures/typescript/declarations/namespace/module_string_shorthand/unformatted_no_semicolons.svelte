<script lang="ts">
	declare module 'a'
	module 'b'
	export declare module 'c'

	declare module 'd' {
		module 'e'
	}
</script>
