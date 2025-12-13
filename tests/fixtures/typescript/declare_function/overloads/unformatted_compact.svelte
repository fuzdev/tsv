<script lang="ts">
	declare function fn(x:string):string;
	declare function fn(x:number):number;

	declare function fn2(x:string):string;
	declare function fn2(x:number):number;
	declare function fn2(x:boolean):boolean;

	declare function fn3<T>(x:T):T;
	declare function fn3<T,U>(x:T,y:U):[T,U];

	declare function fn4(x:string,y?:number):string;
	declare function fn4(x:number,y:string):number;
</script>
