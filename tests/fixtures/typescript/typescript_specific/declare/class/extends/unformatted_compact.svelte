<script lang="ts">
	declare class Single extends Base{prop:string;}

	declare class WithGenerics<T> extends Base<T>{value:T;}

	declare class WithTypeArgs extends Base<string,number>{prop:string;}
</script>
