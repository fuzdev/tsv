<script lang="ts">
	// Non-exported `declare interface`: the `declare` field is emitted before `id`
	declare interface A{a:number;}

	// `export declare interface`: the `declare` field is emitted after `body`
	export declare interface B{b:string;}
</script>
