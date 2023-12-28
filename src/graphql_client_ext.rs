use graphql_client::GraphQLQuery;

/// 重新定义 graphql_client::reqwest::post_graphql_blocking
/// 主要增加了一个观察者闭包函数，观察内部的 header。
pub fn post_graphql_blocking<Q: GraphQLQuery, U: reqwest::IntoUrl>(
    client: &reqwest::blocking::Client,
    url: U,
    variables: Q::Variables,
    // 目前只是一个粗略的实现，由于源库年久失修，这个
    mut f: impl FnMut(&reqwest::header::HeaderMap) -> anyhow::Result<()>,
) -> Result<graphql_client::Response<Q::ResponseData>, reqwest::Error> {
    let body = Q::build_query(variables);
    let reqwest_response = client.post(url).json(&body).send()?;

    // take response headers out
    let _ = f(reqwest_response.headers());

    reqwest_response.json()
}
