<script lang="ts">
	// Exactly 100 chars - stays inline (contrast case)
	export const fn1 = ({a, b, c, d, e = new BackendWebsocketTransport()}: Aaaaaaaaaaaaa): void => {};

	// 101 chars - wraps with destructuring expanded, closing line stays together
	export const fn2 = ({a, b, c, d, e = new BackendWebsocketTransport()}: Aaaaaaaaaaaaaa): void => {};
</script>
