<script lang="ts">
import { faSignOut } from '@fortawesome/free-solid-svg-icons'
import { api } from 'gateway/lib/api'
import { serverInfo, reloadServerInfo } from 'gateway/lib/store'
import Fa from 'svelte-fa'

import logo from '../../public/assets/logo.svg'

import Router, { link } from 'svelte-spa-router'
import active from 'svelte-spa-router/active'
import { wrap } from 'svelte-spa-router/wrap'
import { Spinner } from 'sveltestrap'

async function init () {
    await reloadServerInfo()
}

async function logout () {
    await api.logout()
    await reloadServerInfo()
    location.href = '/@warpgate'
}

init()

const routes = {
    '/': wrap({
        asyncComponent: () => import('./Home.svelte'),
    }),
    '/sessions/:id': wrap({
        asyncComponent: () => import('./Session.svelte'),
    }),
    '/recordings/:id': wrap({
        asyncComponent: () => import('./Recording.svelte'),
    }),
    '/tickets': wrap({
        asyncComponent: () => import('./Tickets.svelte'),
    }),
    '/tickets/create': wrap({
        asyncComponent: () => import('./CreateTicket.svelte'),
    }),
    '/targets': wrap({
        asyncComponent: () => import('./Targets.svelte'),
    }),
    '/ssh': wrap({
        asyncComponent: () => import('./SSH.svelte'),
    }),
    '/log': wrap({
        asyncComponent: () => import('./Log.svelte'),
    }),
}
</script>

{#await init()}
    <Spinner />
{:then}
    <div class="app container">
        <header>
            <a href="/@warpgate" class="d-flex">
                <img class="logo" src={logo} alt="Logo" />
            </a>
            {#if $serverInfo?.username}
                <a use:link use:active href="/">Sessions</a>
                <a use:link use:active href="/targets">Targets</a>
                <a use:link use:active href="/tickets">Tickets</a>
                <a use:link use:active href="/ssh">SSH</a>
                <a use:link use:active href="/log">Log</a>
            {/if}
            {#if $serverInfo?.username}
            <div class="username">
                {$serverInfo?.username}
            </div>
            <button class="btn btn-link" on:click={logout}>
                <Fa icon={faSignOut} fw />
            </button>
            {/if}
        </header>
        <main>
            <Router {routes}/>
        </main>
        <footer>
            {$serverInfo?.version}
        </footer>
    </div>
{/await}

<style lang="scss">
    @import "../vars";

    .app {
        min-height: 100vh;
        display: flex;
        flex-direction: column;
    }

    .logo {
        width: 40px;
        padding-top: 2px;
    }

    header, footer {
        flex: none;
    }

    main {
        flex: 1 0 0;
    }

    header {
        display: flex;
        align-items: center;
        padding: 10px 0;
        margin: 10px 0 20px;
        border-bottom: 1px solid rgba($body-color, .75);

        a, .logo {
            font-size: 1.5rem;
        }

        a:not(:first-child) {
            margin-left: 15px;
        }

        .username {
            margin-left: auto;
        }
    }
</style>
