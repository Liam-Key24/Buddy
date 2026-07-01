import logging

logger = logging.getLogger("buddy.brain.embeddings")

_model = None
_model_name = "sentence-transformers/all-MiniLM-L6-v2"
_dimensions = 384


def _load_model():
    global _model
    if _model is None:
        try:
            from sentence_transformers import SentenceTransformer

            logger.info("loading embedding model: %s", _model_name)
            _model = SentenceTransformer(_model_name)
        except Exception as e:
            logger.warning("sentence-transformers unavailable: %s", e)
            _model = False
    return _model if _model is not False else None


def embed_text(text: str) -> list[float]:
    model = _load_model()
    if model is not None:
        vector = model.encode(text, normalize_embeddings=True)
        return vector.tolist()

    return _hash_fallback(text)


def embedding_dimensions() -> int:
    model = _load_model()
    if model is not None:
        return model.get_sentence_embedding_dimension()
    return _dimensions


def _hash_fallback(text: str) -> list[float]:
    dim = _dimensions
    vec = [0.0] * dim
    for i, byte in enumerate(text.encode("utf-8")):
        vec[i % dim] += byte / 255.0
    norm = sum(v * v for v in vec) ** 0.5
    if norm > 0:
        vec = [v / norm for v in vec]
    return vec
