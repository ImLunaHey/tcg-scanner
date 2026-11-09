import { X } from "lucide-react";
import { Modal } from "./Modal";
import type { CameraResult } from "./Camera";

export function CameraDetails({
  result,
  preview,
  title,
  game,
  language,
  close,
}: {
  result: CameraResult;
  preview?: string;
  title: string;
  game: string;
  language: string;
  close: () => void;
}) {
  return (
    <Modal close={close}>
      <section
        className="camera-card-details"
        role="dialog"
        aria-modal="true"
        aria-labelledby="camera-card-title"
      >
        <button
          className="camera-icon-button camera-details-close"
          autoFocus
          aria-label="Close card details"
          onClick={close}
        >
          <X size={24} />
        </button>
        <div className="camera-card-image">
          {preview && <img src={preview} alt={title} />}
        </div>
        <div className="camera-card-info">
          <p className="eyebrow">{game}</p>
          <h1 id="camera-card-title">{title}</h1>
          {result.candidates.map((card) => (
            <div className="camera-candidate" key={card.id}>
              {result.candidates.length > 1 && <h2>{card.name}</h2>}
              <dl>
                <div>
                  <dt>Set</dt>
                  <dd>{card.setName}</dd>
                </div>
                <div>
                  <dt>Number</dt>
                  <dd>
                    {card.localID}
                    {card.officialSetCount ? `/${card.officialSetCount}` : ""}
                  </dd>
                </div>
                <div>
                  <dt>Language</dt>
                  <dd>{language === "ko" ? "Korean" : "English"}</dd>
                </div>
                {card.category && (
                  <div>
                    <dt>Category</dt>
                    <dd>{card.category}</dd>
                  </div>
                )}
                {!!card.types?.length && (
                  <div>
                    <dt>Type</dt>
                    <dd>{card.types.join(" · ")}</dd>
                  </div>
                )}
                {!!card.rarities?.length && (
                  <div>
                    <dt>Rarity</dt>
                    <dd>{card.rarities.join(" · ")}</dd>
                  </div>
                )}
              </dl>
            </div>
          ))}
          {result.uncataloguedCard && (
            <p>
              {result.uncataloguedCard.setCode} ·{" "}
              {result.uncataloguedCard.collectorNumber} · not in the catalogue
              yet
            </p>
          )}
          {result.state === "codeCard" && (
            <p>Pokémon LIVE redemption insert, not a playable card.</p>
          )}
          {result.energySymbol && !result.candidates.length && (
            <p>Energy type identified; exact printing unknown.</p>
          )}
          <p className="camera-details-note">
            Check the printed set and number. Finish and condition aren’t
            assessed here. Nothing is saved to your collection.
          </p>
          <button className="button" onClick={close}>
            Back to camera
          </button>
        </div>
      </section>
    </Modal>
  );
}
