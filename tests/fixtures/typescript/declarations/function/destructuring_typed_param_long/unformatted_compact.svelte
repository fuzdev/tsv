<script lang="ts">
	// Exactly 100 chars - stays inline (contrast case)
	function fn1({a, b, c, d, e = new BackendWebsocketTransport()}: Aaaaaaaaaaaaaaaaaaaaaa): void {}

	// 101 chars - wraps with destructuring expanded, closing line stays together
	function fn2({a, b, c, d, e = new BackendWebsocketTransport()}: Aaaaaaaaaaaaaaaaaaaaaaa): void {}
</script>
