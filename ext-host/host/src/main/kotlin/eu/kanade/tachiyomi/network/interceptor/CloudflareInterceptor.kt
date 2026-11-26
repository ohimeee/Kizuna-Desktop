package eu.kanade.tachiyomi.network.interceptor

import okhttp3.Interceptor
import okhttp3.Response

/**
 * Kizuna Desktop: a pass-through stand-in, NOT a Cloudflare solver.
 *
 * Keiyoushi extensions refuse to run unless the default client contains an interceptor with this
 * name ("CloudflareInterceptor must be present in default client"). Suwayomi's real version
 * solves challenges through FlareSolverr; Kizuna deliberately doesn't bypass bot checks, so this
 * one hands every request through unchanged. A source behind a Cloudflare challenge fails as it
 * would without it.
 */
class CloudflareInterceptor : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response = chain.proceed(chain.request())
}
