from main import BRAIN_API, BRAIN_ROUTES, app, health


def test_health_lists_talk():
    body = health()
    assert body["status"] == "ok"
    assert body["api"] >= 2
    assert body["api"] == BRAIN_API
    assert "/chat/talk" in body["routes"]
    assert "/v1/complete" in body["routes"]
    assert "/embed" in body["routes"]
    assert body["routes"] == list(BRAIN_ROUTES)


def test_talk_route_registered():
    paths = {getattr(route, "path", "") for route in app.routes}
    assert "/health" in paths
    assert "/chat/talk" in paths
    assert "/chat/respond" in paths
    assert "/v1/complete" in paths
