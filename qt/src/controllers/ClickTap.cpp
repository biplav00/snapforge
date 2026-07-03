#include "ClickTap.h"

#include "SnapforgeClient.h"

ClickTap::ClickTap(QObject *parent)
    : QObject(parent) {}

ClickTap::~ClickTap() {
    stop();
}

bool ClickTap::isRunning() const {
    return m_handle.valid();
}

bool ClickTap::start() {
    if (m_handle.valid()) return true;
    m_handle = sf::clicksStart(&ClickTap::onClickStatic, this);
    return m_handle.valid();
}

void ClickTap::stop() {
    if (!m_handle.valid()) return;
    sf::clicksStop(m_handle);
    sf::clicksFreeHandle(m_handle);
    m_handle = sf::ClickHandle{};
}

void ClickTap::onClickStatic(double /*x*/, double /*y*/, int /*rightClick*/,
                             void * /*userData*/) {
    // No-op: click ripples are baked into the recording by the Rust encoder.
    // ClickTap only installs the tap to probe the permission grant, so the
    // per-click events are intentionally dropped here.
}
