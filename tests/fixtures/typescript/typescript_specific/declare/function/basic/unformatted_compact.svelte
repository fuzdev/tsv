<script lang="ts">
	declare function empty():void;

	declare function single(x:number):number;

	declare function multiple(a:string,b:number,c:boolean):void;

	declare function optional(required:string,opt?:number):void;

	declare function rest(...args:string[]):void;

	declare function returns():string;

	declare function withThis(this:Window):void;

	declare function isString(x:unknown):x is string;

	declare function assertString(x:unknown):asserts x is string;

	declare function assertVal(x:unknown):asserts x;

	declare function assertThis():asserts this;

	export declare function exported():void;
</script>
