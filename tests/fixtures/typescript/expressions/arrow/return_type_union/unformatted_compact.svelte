<script lang="ts">
// Short union - stays inline
export const fn1 = (first_param_with_long_name:string,second_param_with_long_name:number):string|null=>{return null;};

// Longer union - still inline (fits)
export const fn2 = (x:Record<string,any>|null|undefined):string|null|undefined|boolean=>{return null;};

// Very long union - breaks when exceeds width
export const fn3=(x:Record<string,any>|null|undefined):VeryLongTypeName|AnotherVeryLongTypeName|YetAnotherVeryLongTypeName|FinalVeryLongTypeName=>{return null;};
</script>
