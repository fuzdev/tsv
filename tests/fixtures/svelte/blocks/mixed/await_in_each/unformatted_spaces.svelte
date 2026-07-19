{#each  promises  as  promise  }
   {#await  promise  then  value  }
      <div  >{value}</div  >
   {/await }
{/each }
