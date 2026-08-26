export default {
  async fetch(request, env) {
    const upstreamOrigin = new URL(env.UPSTREAM_ORIGIN);
    const upstream = new URL(request.url);
    upstream.protocol = upstreamOrigin.protocol;
    upstream.port = upstreamOrigin.port;
    upstream.hostname = upstreamOrigin.hostname;

    return fetch(new Request(upstream, request));
  },
};
