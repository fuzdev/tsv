<script>
async function fn(){
// C-style for: block comment between keyword and first declarator
for(let/* c */i=0;i<n;i++){a();}

// C-style for, multiple declarators: block comment on the keyword branch (group path)
for(let/* c */p=0,q=1;p<n;p++){b();}

// for-of: block comment between keyword and binding
for(const/* c */x of items){c();}

// for-in: block comment between keyword and binding
for(const/* c */k in obj){d();}

// for-await-of: block comment between keyword and binding
for await(const/* c */y of items){e();}
}
</script>
